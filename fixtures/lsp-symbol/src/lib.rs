pub fn resolved_port() -> u16 {
    8080
}

pub fn primary_port() -> u16 {
    resolved_port()
}

#[cfg(test)]
mod tests {
    use super::resolved_port;

    #[test]
    fn port_is_stable() {
        assert_eq!(resolved_port(), 8080);
    }
}
