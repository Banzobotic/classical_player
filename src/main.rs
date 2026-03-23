use crate::model::Model;

mod table;
mod model;

fn main() {
    let redis_client = redis::Client::open("redis://127.0.0.1/").unwrap();
    Model::new(redis_client).run();    
}
