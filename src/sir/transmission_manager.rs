use std::collections::HashMap;

use eosim::{
    context::{Component, Context, PlanId},
    global_properties::GlobalPropertyContext,
    people::PersonId,
    person_properties::PersonPropertyContext,
    random::{RandomContext, RandomId},
};
use rand::Rng;
use rand_distr::{Distribution, Exp};
use rand_xoshiro::Xoshiro256PlusPlus;

use super::{
    global_properties::{InfectiousPeriod, Population, R0},
    person_properties::DiseaseStatus,
};

pub struct TransmissionManager {}

impl Component for TransmissionManager {
    fn init(context: &mut Context) {
        context
            .observe_person_property_changes::<DiseaseStatus>(handle_person_disease_status_change);
    }
}

eosim::define_plugin!(TransmissionManagerPlugin, HashMap<PersonId, PlanId>, HashMap::new());
pub struct TransmissionRandomId {}

impl RandomId for TransmissionRandomId {
    type RngType = Xoshiro256PlusPlus;

    fn seed_offset() -> u64 {
        fxhash::hash64("TransmissionRandomId")
    }
}

pub fn handle_person_disease_status_change(
    context: &mut Context,
    person_id: PersonId,
    _: DiseaseStatus,
) {
    let disease_status = context.get_person_property_value::<DiseaseStatus>(person_id);
    match disease_status {
        DiseaseStatus::I => schedule_next_infectious_contact(context, person_id),
        DiseaseStatus::R => cancel_next_infectious_contact(context, person_id),
        _ => {
            println!("{}", context.get_time())
        }
    }
}

fn schedule_next_infectious_contact(context: &mut Context, person_id: PersonId) {
    let r0 = context
        .get_global_property_value::<R0>()
        .expect("R0 not specified");
    let infectious_period = context
        .get_global_property_value::<InfectiousPeriod>()
        .expect("Infectious period not specified");
    let contact_rate_dist = Exp::new(r0 / infectious_period).unwrap();
    let next_contact_time = context.get_time()
        + contact_rate_dist.sample(&mut *context.get_rng::<TransmissionRandomId>());
    let contact_plan = context.add_plan(next_contact_time, move |context| {
        attempt_infection(context, person_id)
    });
    // Store plan id for future use (cancelling upon recovery)
    context
        .get_data_container_mut::<TransmissionManagerPlugin>()
        .insert(person_id, contact_plan);
}

fn attempt_infection(context: &mut Context, source_person_id: PersonId) {
    let population = *context
        .get_global_property_value::<Population>()
        .expect("Population not specified");
    if population > 1 {
        let mut contact_id;
        let mut rng = context.get_rng::<TransmissionRandomId>();
        loop {
            contact_id = PersonId::new((*rng).gen_range(0..(population + 1)));
            if contact_id != source_person_id {
                break;
            }
        }
        drop(rng);
        let contact_disease_status = context.get_person_property_value::<DiseaseStatus>(contact_id);
        if matches!(contact_disease_status, DiseaseStatus::S) {
            context.set_person_property_value::<DiseaseStatus>(contact_id, DiseaseStatus::I)
        }
        schedule_next_infectious_contact(context, source_person_id)
    }
}

fn cancel_next_infectious_contact(context: &mut Context, person_id: PersonId) {
    let contact_plan = context
        .get_data_container_mut::<TransmissionManagerPlugin>()
        .remove(&person_id);
    if let Some(contact_plan) = contact_plan {
        context.cancel_plan(contact_plan);
    }
}
