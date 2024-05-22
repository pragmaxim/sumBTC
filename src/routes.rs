use actix_web::{get, web, Responder};

#[get("/hello/{name}")]
async fn greet(name: web::Path<String>) -> impl Responder {
    /*
    for (address, balance) in merkle_sum_tree
           .top_richest_address()
           .unwrap()
           .iter()
           .take(10)
       {
           println!(
               "Address: {}, Balance: {}",
               std::str::from_utf8(&address).unwrap(),
               balance
           );
       }
    */
    format!("Hello {name}!")
}
