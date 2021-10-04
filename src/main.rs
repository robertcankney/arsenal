mod shim;

use shim::{cache, proxy};

fn main(){

    let c = cache::Cache;
    let p = proxy::Handler;
}
