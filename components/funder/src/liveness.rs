use im::hashset::HashSet as ImHashSet;

use proto::crypto::PublicKey;

#[derive(Clone, Default)]
pub struct Liveness {
    pub friends: ImHashSet<PublicKey>,
}

#[derive(Debug)]
pub enum LivenessMutation {
    SetOnline(PublicKey),
    SetOffline(PublicKey),
}

impl Liveness {
    pub fn new() -> Liveness {
        Liveness {
            friends: ImHashSet::new(),
        }
    }

    pub fn mutate(&mut self, mutation: &LivenessMutation) {
        match mutation {
            LivenessMutation::SetOnline(public_key) => {
                self.friends.insert(public_key.clone());
            }
            LivenessMutation::SetOffline(public_key) => {
                let _ = self.friends.remove(public_key);
            }
        }
    }

    pub fn is_online(&self, friend_public_key: &PublicKey) -> bool {
        self.friends.contains(&friend_public_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_liveness_basic() {
        let mut liveness = Liveness::new();
        let pk_a = PublicKey::from(&[0xaa; PublicKey::len()]);
        let pk_b = PublicKey::from(&[0xbb; PublicKey::len()]);
        let pk_c = PublicKey::from(&[0xcc; PublicKey::len()]);

        assert!(!liveness.is_online(&pk_a));
        assert!(!liveness.is_online(&pk_b));
        assert!(!liveness.is_online(&pk_c));

        liveness.mutate(&LivenessMutation::SetOnline(pk_a.clone()));
        assert!(liveness.is_online(&pk_a));
        assert!(!liveness.is_online(&pk_b));
        assert!(!liveness.is_online(&pk_c));

        liveness.mutate(&LivenessMutation::SetOnline(pk_a.clone()));
        assert!(liveness.is_online(&pk_a));
        assert!(!liveness.is_online(&pk_b));
        assert!(!liveness.is_online(&pk_c));

        liveness.mutate(&LivenessMutation::SetOnline(pk_b.clone()));
        assert!(liveness.is_online(&pk_a));
        assert!(liveness.is_online(&pk_b));
        assert!(!liveness.is_online(&pk_c));

        liveness.mutate(&LivenessMutation::SetOffline(pk_c.clone()));
        assert!(liveness.is_online(&pk_a));
        assert!(liveness.is_online(&pk_b));
        assert!(!liveness.is_online(&pk_c));

        liveness.mutate(&LivenessMutation::SetOffline(pk_b.clone()));
        assert!(liveness.is_online(&pk_a));
        assert!(!liveness.is_online(&pk_b));
        assert!(!liveness.is_online(&pk_c));

        liveness.mutate(&LivenessMutation::SetOffline(pk_b.clone()));
        assert!(liveness.is_online(&pk_a));
        assert!(!liveness.is_online(&pk_b));
        assert!(!liveness.is_online(&pk_c));

        liveness.mutate(&LivenessMutation::SetOffline(pk_a.clone()));
        assert!(!liveness.is_online(&pk_a));
        assert!(!liveness.is_online(&pk_b));
        assert!(!liveness.is_online(&pk_c));
    }
}
