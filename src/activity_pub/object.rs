use activity_pub::actor::Actor;

pub trait Object {}

pub trait Attribuable {
    fn set_attribution<T>(&self, by: &T) where T: Actor;
}
