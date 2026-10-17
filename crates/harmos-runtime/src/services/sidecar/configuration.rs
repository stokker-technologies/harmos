/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

/* === Imports === */
use super::*;

/* === Implementations === */
impl DemandSource {
    pub fn hold(&self, key: &str) -> Result<DemandLease, SidecarError> {
        let count = self
            .keys
            .get(key)
            .ok_or_else(|| SidecarError::Malformed(format!("undeclared demand key `{key}`")))?
            .clone();
        count.send_modify(|count| *count += 1);

        Ok(DemandLease { count })
    }
}

impl Drop for DemandLease {
    fn drop(&mut self) {
        self.count.send_modify(|count| *count -= 1);
    }
}

impl<A: Application> Clone for Inbound<A> {
    fn clone(&self) -> Self {
        Self {
            streams: self.streams.clone(),
            demands: self.demands.clone(),
        }
    }
}

impl<A: Application> Default for Inbound<A> {
    fn default() -> Self {
        Self {
            streams: BTreeMap::new(),
            demands: BTreeMap::new(),
        }
    }
}

impl<A: Application, S: Attach, H> Supervised<A, S, H> {
    pub fn new(attach: S, notice: H) -> Self {
        let (registrations, _) = tokio::sync::watch::channel(None);
        Self {
            attach,
            notice: Arc::new(notice),
            ingress: Arc::new(Inbound::default()),
            demand_keys: Arc::new(BTreeMap::new()),
            expected_registrations: None,
            handshake: SETTLING,
            grace: SETTLING,
            calls: Arc::new(Calls::new(registrations.clone())),
            registrations,
            routing: None,
            manager_owned_stop: false,
            cleanup: tokio::sync::Mutex::new(None),
        }
    }

    pub fn demand_keys(mut self, keys: impl IntoIterator<Item = String>) -> Self {
        let publications = self.attach.publications();
        let mut admitted = BTreeMap::new();
        for key in keys {
            assert!(
                key.split_once(':')
                    .is_some_and(|(namespace, name)| !namespace.is_empty() && !name.is_empty()),
                "demand keys must be namespaced"
            );
            assert!(
                !publications.iter().any(|entry| entry.id == key),
                "demand keys must differ from publication identities"
            );
            admitted
                .entry(key)
                .or_insert_with(|| tokio::sync::watch::channel(0).0);
        }

        self.demand_keys = Arc::new(admitted);
        self
    }

    pub fn demands(&self) -> DemandSource {
        DemandSource {
            keys: Arc::clone(&self.demand_keys),
        }
    }

    pub fn routed(mut self, router: &crate::plugin::Router) -> Self {
        self.routing = Some(router.clone());
        self
    }

    pub fn registrations(&self) -> tokio::sync::watch::Receiver<Option<Vec<Registration>>> {
        self.registrations.subscribe()
    }

    pub fn expected_registrations(mut self, registrations: &[Registration]) -> Self {
        let mut registrations = registrations.to_vec();
        registrations.sort_by(|left, right| left.namespace.cmp(&right.namespace));
        self.expected_registrations = Some(Arc::new(registrations));
        self
    }

    pub fn ready(&self) -> tokio::sync::watch::Receiver<bool> {
        self.calls.ready.subscribe()
    }

    pub fn ingest<Source>(mut self) -> Self
    where
        Source: harmos_sidecar::StreamSpec,
        Source::Row: Stream<A> + Message + Default,
    {
        let ingress = Arc::make_mut(&mut self.ingress);
        ingress
            .demands
            .insert(Source::ID, |streams: &StreamManager<A>| {
                streams.demand::<Source::Row>()
            });
        let streams = &mut ingress.streams;
        let previous = streams.insert(
            Source::ID,
            Arc::new(|context, publication| {
                if publication.schema() != Source::VERSION {
                    return Err(format!(
                        "sidecar stream `{}` sent schema {}, expected {}",
                        Source::ID,
                        publication.schema(),
                        Source::VERSION
                    ));
                }
                let row = Source::Row::decode(publication.payload()).map_err(|error| {
                    format!(
                        "sidecar stream `{}` sent malformed protobuf: {error}",
                        Source::ID
                    )
                })?;
                context.streams().miss::<Source::Row>(publication.missed());
                context.streams().publish(row);

                Ok(())
            }),
        );
        assert!(
            previous.is_none(),
            "sidecar stream `{}` was registered twice",
            Source::ID
        );

        self
    }

    pub fn handshake(mut self, within: Duration) -> Self {
        self.handshake = within;
        self
    }

    pub fn grace(mut self, within: Duration) -> Self {
        self.grace = within;
        self
    }

    pub async fn staged_operations(&self) -> Result<crate::operations::Native, SidecarError> {
        let mut staged = self.calls.staged.subscribe();
        loop {
            if *staged.borrow_and_update() {
                return self.calls.current().map(crate::operations::Native::new);
            }
            staged
                .changed()
                .await
                .map_err(|_| SidecarError::Unavailable)?;
        }
    }

    pub fn operations(&self) -> Result<crate::operations::Native, SidecarError> {
        let endpoint = self.calls.current()?;
        if !*endpoint.initialized().borrow()
            || !endpoint.scope().is_some_and(|scope| scope.is_ready())
        {
            return Err(SidecarError::Unavailable);
        }
        Ok(crate::operations::Native::new(endpoint))
    }
}

impl<A: Application, S: Clone, H> Clone for Supervised<A, S, H> {
    fn clone(&self) -> Self {
        Self {
            attach: self.attach.clone(),
            notice: Arc::clone(&self.notice),
            ingress: Arc::clone(&self.ingress),
            demand_keys: Arc::clone(&self.demand_keys),
            expected_registrations: self.expected_registrations.clone(),
            handshake: self.handshake,
            grace: self.grace,
            calls: Arc::clone(&self.calls),
            registrations: self.registrations.clone(),
            routing: self.routing.clone(),
            manager_owned_stop: self.manager_owned_stop,
            cleanup: tokio::sync::Mutex::new(None),
        }
    }
}

