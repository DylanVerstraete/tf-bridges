mod stellar;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        stellar::Client::new("seed", stellar::Network::new_public()).expect("msg");
    }
}
