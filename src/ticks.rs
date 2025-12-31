use bevy_app::{App, Last, Plugin};
use bevy_ecs::{resource::Resource, system::ResMut};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

/// A struct keeping track of how many ticks have elapsed since the start of the program.
#[derive(Resource)]
pub struct UpdateTicks {
    ticks: Arc<AtomicUsize>,
    tick_tx: tokio::sync::watch::Sender<()>,
}

impl UpdateTicks {
    pub fn tick(&self) -> usize {
        self.ticks.load(Ordering::SeqCst)
    }

    pub fn ticks(&self) -> Arc<AtomicUsize> {
        self.ticks.clone()
    }

    pub fn tick_rx(&self) -> tokio::sync::watch::Receiver<()> {
        self.tick_tx.subscribe()
    }

    fn increment_ticks(&self) -> usize {
        let new_ticks = self.ticks.fetch_add(1, Ordering::SeqCst).wrapping_add(1);
        let _ = self.tick_tx.send(());
        new_ticks
    }
}

pub struct TicksPlugin;

impl TicksPlugin {
    fn increment_system(ticks: ResMut<UpdateTicks>) {
        ticks.increment_ticks();
        // // Run as late as possible, by running in a command after Last.
        // commands.add(|world: &mut World| {
        //     world
        //         .get_resource_mut::<UpdateTicks>()
        //         .unwrap()
        //         .increment_ticks();
        // });
    }
}

impl Plugin for TicksPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(UpdateTicks {
            ticks: Arc::new(AtomicUsize::new(0)),
            tick_tx: tokio::sync::watch::channel(()).0,
        })
        .add_systems(Last, Self::increment_system);
    }
}
