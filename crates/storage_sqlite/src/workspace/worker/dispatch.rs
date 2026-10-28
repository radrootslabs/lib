//! Each job has an isolated polling frame on the existing bounded worker.
use super::*;
use core::{future::Future, pin::Pin};

pub(super) fn run<'a>(
    job: Job,
    kind: Kind,
    turn: stock::inventory::Turn<'a>,
) -> Pin<Box<dyn Future<Output = ()> + 'a>> {
    let stock::inventory::Turn {
        connection,
        namespace,
        selected: input,
        crypto,
        clock,
        uncertain,
    } = turn;
    match job {
        Job::ChangeStockGrant(change, proof, reply) => Box::pin(async move {
            let result = if kind == Kind::SellerStock && !*uncertain {
                stock::grants::change(
                    &mut stock::inventory::Turn {
                        connection,
                        namespace,
                        selected: input,
                        crypto,
                        clock,
                        uncertain,
                    },
                    *change,
                    proof,
                )
                .await
            } else {
                Err(WorkspaceError::RecoveryRequired)
            };
            let _ = reply.send(result);
        }),
        Job::ExecuteInventory(request, proof, reply) => Box::pin(async move {
            let result = if kind == Kind::SellerStock {
                stock::inventory::Turn {
                    connection,
                    namespace,
                    selected: input,
                    crypto,
                    clock,
                    uncertain,
                }
                .create(*request, proof)
                .await
            } else {
                Err(OperationError::UnsupportedOperation)
            };
            let _ = reply.send(result);
        }),
        Job::InventoryOperation(binding, minimum, operation, digest, proof, reply) => {
            Box::pin(async move {
                let result = if kind == Kind::SellerStock {
                    stock::inventory::Turn {
                        connection,
                        namespace,
                        selected: input,
                        crypto,
                        clock,
                        uncertain,
                    }
                    .creation(*binding, minimum, operation, digest, proof)
                    .await
                } else {
                    Err(OperationError::UnsupportedOperation)
                };
                let _ = reply.send(result);
            })
        }
        Job::IssueStockConsent(binding, minimum, capture, proof, reply) => Box::pin(async move {
            let result = if kind == Kind::SellerStock {
                stock::consent::issue(
                    &mut stock::inventory::Turn {
                        connection,
                        namespace,
                        selected: input,
                        crypto,
                        clock,
                        uncertain,
                    },
                    *binding,
                    minimum,
                    *capture,
                    proof,
                )
                .await
            } else {
                Err(OperationError::UnsupportedOperation)
            };
            let _ = reply.send(result);
        }),
        Job::StockConsent(binding, minimum, operation, subject, proof, reply) => {
            Box::pin(async move {
                let result = if kind == Kind::SellerStock {
                    stock::consent::query(
                        &mut stock::inventory::Turn {
                            connection,
                            namespace,
                            selected: input,
                            crypto,
                            clock,
                            uncertain,
                        },
                        *binding,
                        minimum,
                        operation,
                        subject,
                        proof,
                    )
                    .await
                } else {
                    Err(OperationError::UnsupportedOperation)
                };
                let _ = reply.send(result);
            })
        }
        Job::RetainStockListing(binding, minimum, capture, proof, reply) => Box::pin(async move {
            let result = if kind == Kind::SellerStock {
                stock::public_source::retain(
                    &mut stock::inventory::Turn {
                        connection,
                        namespace,
                        selected: input,
                        crypto,
                        clock,
                        uncertain,
                    },
                    *binding,
                    minimum,
                    *capture,
                    proof,
                )
                .await
            } else {
                Err(OperationError::UnsupportedOperation)
            };
            let _ = reply.send(result);
        }),
        Job::StockListing(binding, minimum, lookup, proof, reply) => Box::pin(async move {
            let result = if kind == Kind::SellerStock {
                stock::public_source::query(
                    &mut stock::inventory::Turn {
                        connection,
                        namespace,
                        selected: input,
                        crypto,
                        clock,
                        uncertain,
                    },
                    *binding,
                    minimum,
                    lookup,
                    proof,
                )
                .await
            } else {
                Err(OperationError::UnsupportedOperation)
            };
            let _ = reply.send(result);
        }),
        Job::RetainStockSource(binding, minimum, capture, proof, reply) => Box::pin(async move {
            let result = if kind == Kind::SellerStock {
                stock::source::retain(
                    &mut stock::inventory::Turn {
                        connection,
                        namespace,
                        selected: input,
                        crypto,
                        clock,
                        uncertain,
                    },
                    *binding,
                    minimum,
                    *capture,
                    proof,
                )
                .await
            } else {
                Err(OperationError::UnsupportedOperation)
            };
            let _ = reply.send(result);
        }),
        Job::InstallStockAgreement(request, proof, reply) => Box::pin(async move {
            let result = if kind == Kind::SellerStock {
                Box::pin(stock::inventory::installation::install(
                    &mut stock::inventory::Turn {
                        connection,
                        namespace,
                        selected: input,
                        crypto,
                        clock,
                        uncertain,
                    },
                    *request,
                    proof,
                ))
                .await
            } else {
                Err(OperationError::UnsupportedOperation)
            };
            let _ = reply.send(result);
        }),
        Job::StockAgreementInstallation(request, proof, reply) => Box::pin(async move {
            let result = if kind == Kind::SellerStock {
                Box::pin(stock::inventory::installation::read(
                    &mut stock::inventory::Turn {
                        connection,
                        namespace,
                        selected: input,
                        crypto,
                        clock,
                        uncertain,
                    },
                    *request,
                    proof,
                ))
                .await
            } else {
                Err(OperationError::UnsupportedOperation)
            };
            let _ = reply.send(result);
        }),
        Job::AbortStockPreparation(request, proof, reply) => Box::pin(async move {
            let result = if kind == Kind::SellerStock {
                Box::pin(stock::inventory::abort::abort(
                    &mut stock::inventory::Turn {
                        connection,
                        namespace,
                        selected: input,
                        crypto,
                        clock,
                        uncertain,
                    },
                    *request,
                    proof,
                ))
                .await
            } else {
                Err(OperationError::UnsupportedOperation)
            };
            let _ = reply.send(result);
        }),
        Job::StockPreparationAbort(request, proof, reply) => Box::pin(async move {
            let result = if kind == Kind::SellerStock {
                Box::pin(stock::inventory::abort::read(
                    &mut stock::inventory::Turn {
                        connection,
                        namespace,
                        selected: input,
                        crypto,
                        clock,
                        uncertain,
                    },
                    *request,
                    proof,
                ))
                .await
            } else {
                Err(OperationError::UnsupportedOperation)
            };
            let _ = reply.send(result);
        }),
        Job::StockSnapshot(request, proof, reply) => Box::pin(async move {
            let result = if kind == Kind::SellerStock {
                Box::pin(stock::snapshot::read(
                    &mut stock::inventory::Turn {
                        connection,
                        namespace,
                        selected: input,
                        crypto,
                        clock,
                        uncertain,
                    },
                    *request,
                    proof,
                ))
                .await
            } else {
                Err(OperationError::UnsupportedOperation)
            };
            let _ = reply.send(result);
        }),
        Job::StockEvidencePage(request, after, proof, reply) => Box::pin(async move {
            let result = if kind == Kind::SellerStock {
                Box::pin(stock::issuance::evidence::page(
                    &mut stock::inventory::Turn {
                        connection,
                        namespace,
                        selected: input,
                        crypto,
                        clock,
                        uncertain,
                    },
                    *request,
                    after,
                    proof,
                ))
                .await
            } else {
                Err(OperationError::UnsupportedOperation)
            };
            let _ = reply.send(result);
        }),
        Job::CaptureStockUnissued(request, proof, reply) => Box::pin(async move {
            let result = if kind == Kind::SellerStock {
                Box::pin(stock::issuance::unissued::capture(
                    &mut stock::inventory::Turn {
                        connection,
                        namespace,
                        selected: input,
                        crypto,
                        clock,
                        uncertain,
                    },
                    *request,
                    proof,
                ))
                .await
            } else {
                Err(OperationError::UnsupportedOperation)
            };
            let _ = reply.send(result);
        }),
        Job::StockUnissued(request, proof, reply) => Box::pin(async move {
            let result = if kind == Kind::SellerStock {
                Box::pin(stock::issuance::unissued::read(
                    &mut stock::inventory::Turn {
                        connection,
                        namespace,
                        selected: input,
                        crypto,
                        clock,
                        uncertain,
                    },
                    *request,
                    proof,
                ))
                .await
            } else {
                Err(OperationError::UnsupportedOperation)
            };
            let _ = reply.send(result);
        }),
        Job::PrepareStockDelivery(request, proof, reply) => Box::pin(async move {
            let result = if kind == Kind::SellerStock {
                Box::pin(stock::delivery::prepare(
                    &mut stock::inventory::Turn {
                        connection,
                        namespace,
                        selected: input,
                        crypto,
                        clock,
                        uncertain,
                    },
                    *request,
                    proof,
                ))
                .await
            } else {
                Err(OperationError::UnsupportedOperation)
            };
            let _ = reply.send(result);
        }),
        Job::StockDeliveryAttempt(request, request_id, proof, reply) => Box::pin(async move {
            let result = if kind == Kind::SellerStock {
                Box::pin(stock::delivery::query_attempt(
                    &mut stock::inventory::Turn {
                        connection,
                        namespace,
                        selected: input,
                        crypto,
                        clock,
                        uncertain,
                    },
                    *request,
                    request_id,
                    proof,
                ))
                .await
            } else {
                Err(OperationError::UnsupportedOperation)
            };
            let _ = reply.send(result);
        }),
        Job::StockDeliverySummary(request, proof, reply) => Box::pin(async move {
            let result = if kind == Kind::SellerStock {
                Box::pin(stock::delivery::summary(
                    &mut stock::inventory::Turn {
                        connection,
                        namespace,
                        selected: input,
                        crypto,
                        clock,
                        uncertain,
                    },
                    *request,
                    proof,
                ))
                .await
            } else {
                Err(OperationError::UnsupportedOperation)
            };
            let _ = reply.send(result);
        }),
        Job::RecordStockDeliveryObservation(binding, proof, observation, reply) => {
            Box::pin(async move {
                let result = if kind == Kind::SellerStock {
                    Box::pin(stock::delivery::capture_observation(
                        &mut stock::inventory::Turn {
                            connection,
                            namespace,
                            selected: input,
                            crypto,
                            clock,
                            uncertain,
                        },
                        *binding,
                        proof,
                        *observation,
                    ))
                    .await
                } else {
                    Err(OperationError::UnsupportedOperation)
                };
                let _ = reply.send(result);
            })
        }
        Job::StockDeliveryObservations(request, after, proof, reply) => Box::pin(async move {
            let result = if kind == Kind::SellerStock {
                Box::pin(stock::delivery::page(
                    &mut stock::inventory::Turn {
                        connection,
                        namespace,
                        selected: input,
                        crypto,
                        clock,
                        uncertain,
                    },
                    *request,
                    after,
                    proof,
                ))
                .await
            } else {
                Err(OperationError::UnsupportedOperation)
            };
            let _ = reply.send(result);
        }),
        Job::CaptureStockEnvelope(request, proof, candidate, reply) => Box::pin(async move {
            let result = if kind == Kind::SellerStock {
                Box::pin(stock::envelope::capture(
                    &mut stock::inventory::Turn {
                        connection,
                        namespace,
                        selected: input,
                        crypto,
                        clock,
                        uncertain,
                    },
                    *request,
                    proof,
                    *candidate,
                ))
                .await
            } else {
                Err(OperationError::UnsupportedOperation)
            };
            let _ = reply.send(result);
        }),
        Job::StockEnvelope(request, proof, reply) => Box::pin(async move {
            let result = if kind == Kind::SellerStock {
                Box::pin(stock::envelope::read(
                    &mut stock::inventory::Turn {
                        connection,
                        namespace,
                        selected: input,
                        crypto,
                        clock,
                        uncertain,
                    },
                    *request,
                    proof,
                ))
                .await
            } else {
                Err(OperationError::UnsupportedOperation)
            };
            let _ = reply.send(result);
        }),
        Job::CaptureStockEvidence(evidence, proof, reply) => Box::pin(async move {
            let result = if kind == Kind::SellerStock {
                Box::pin(stock::issuance::evidence::capture(
                    &mut stock::inventory::Turn {
                        connection,
                        namespace,
                        selected: input,
                        crypto,
                        clock,
                        uncertain,
                    },
                    *evidence,
                    proof,
                ))
                .await
            } else {
                Err(OperationError::UnsupportedOperation)
            };
            let _ = reply.send(result);
        }),
        Job::StockEvidence(request, digest, proof, reply) => Box::pin(async move {
            let result = if kind == Kind::SellerStock {
                Box::pin(stock::issuance::evidence::read(
                    &mut stock::inventory::Turn {
                        connection,
                        namespace,
                        selected: input,
                        crypto,
                        clock,
                        uncertain,
                    },
                    *request,
                    digest,
                    proof,
                ))
                .await
            } else {
                Err(OperationError::UnsupportedOperation)
            };
            let _ = reply.send(result);
        }),
        Job::ClaimStockSigning(request, proof, reply) => Box::pin(async move {
            let result = if kind == Kind::SellerStock {
                Box::pin(stock::issuance::claim::claim(
                    &mut stock::inventory::Turn {
                        connection,
                        namespace,
                        selected: input,
                        crypto,
                        clock,
                        uncertain,
                    },
                    *request,
                    proof,
                ))
                .await
            } else {
                Err(OperationError::UnsupportedOperation)
            };
            let _ = reply.send(result);
        }),
        Job::AuthorizeStockExactReplay(request, proof, reply) => Box::pin(async move {
            let result = if kind == Kind::SellerStock {
                Box::pin(stock::issuance::recovery::authorize(
                    &mut stock::inventory::Turn {
                        connection,
                        namespace,
                        selected: input,
                        crypto,
                        clock,
                        uncertain,
                    },
                    *request,
                    proof,
                ))
                .await
            } else {
                Err(OperationError::UnsupportedOperation)
            };
            let _ = reply.send(result);
        }),
        Job::StockSigningAttempt(request, proof, reply) => Box::pin(async move {
            let result = if kind == Kind::SellerStock {
                Box::pin(stock::issuance::claim::read(
                    &mut stock::inventory::Turn {
                        connection,
                        namespace,
                        selected: input,
                        crypto,
                        clock,
                        uncertain,
                    },
                    *request,
                    proof,
                ))
                .await
            } else {
                Err(OperationError::UnsupportedOperation)
            };
            let _ = reply.send(result);
        }),
        Job::BindStockSigningPlan(request, proof, reply) => Box::pin(async move {
            let result = if kind == Kind::SellerStock {
                Box::pin(stock::signing_plan::bind(
                    &mut stock::inventory::Turn {
                        connection,
                        namespace,
                        selected: input,
                        crypto,
                        clock,
                        uncertain,
                    },
                    *request,
                    proof,
                ))
                .await
            } else {
                Err(OperationError::UnsupportedOperation)
            };
            let _ = reply.send(result);
        }),
        Job::StockSigningPlan(binding, minimum, operation, digest, proof, reply) => {
            Box::pin(async move {
                let result = if kind == Kind::SellerStock {
                    Box::pin(stock::signing_plan::read(
                        &mut stock::inventory::Turn {
                            connection,
                            namespace,
                            selected: input,
                            crypto,
                            clock,
                            uncertain,
                        },
                        *binding,
                        minimum,
                        operation,
                        digest,
                        proof,
                    ))
                    .await
                } else {
                    Err(OperationError::UnsupportedOperation)
                };
                let _ = reply.send(result);
            })
        }
        Job::PrepareStockAcceptance(request, proof, reply) => Box::pin(async move {
            let result = if kind == Kind::SellerStock {
                Box::pin(stock::inventory::acceptance::admit(
                    &mut stock::inventory::Turn {
                        connection,
                        namespace,
                        selected: input,
                        crypto,
                        clock,
                        uncertain,
                    },
                    *request,
                    proof,
                ))
                .await
            } else {
                Err(OperationError::UnsupportedOperation)
            };
            let _ = reply.send(result);
        }),
        Job::StockOrderRequest(
            binding,
            inventory_minimum,
            minimum,
            listings,
            selected,
            proof,
            reply,
        ) => Box::pin(async move {
            let result = if kind == Kind::SellerStock {
                Box::pin(stock::source::inspect_request(
                    &mut stock::inventory::Turn {
                        connection,
                        namespace,
                        selected: input,
                        crypto,
                        clock,
                        uncertain,
                    },
                    *binding,
                    inventory_minimum,
                    minimum,
                    listings,
                    selected,
                    proof,
                ))
                .await
            } else {
                Err(OperationError::UnsupportedOperation)
            };
            let _ = reply.send(result);
        }),
        Job::StockOrderHistory(
            binding,
            inventory_minimum,
            minimum,
            listings,
            scope,
            proof,
            reply,
        ) => Box::pin(async move {
            let result = if kind == Kind::SellerStock {
                Box::pin(stock::source::query_history(
                    &mut stock::inventory::Turn {
                        connection,
                        namespace,
                        selected: input,
                        crypto,
                        clock,
                        uncertain,
                    },
                    *binding,
                    inventory_minimum,
                    minimum,
                    listings,
                    scope,
                    proof,
                ))
                .await
            } else {
                Err(OperationError::UnsupportedOperation)
            };
            let _ = reply.send(result);
        }),
        Job::StockSource(binding, minimum, envelope, proof, reply) => Box::pin(async move {
            let result = if kind == Kind::SellerStock {
                stock::source::query(
                    &mut stock::inventory::Turn {
                        connection,
                        namespace,
                        selected: input,
                        crypto,
                        clock,
                        uncertain,
                    },
                    *binding,
                    minimum,
                    envelope,
                    proof,
                )
                .await
            } else {
                Err(OperationError::UnsupportedOperation)
            };
            let _ = reply.send(result);
        }),
        Job::OptionMapping(binding, minimum, key, proof, reply) => Box::pin(async move {
            let result = if kind == Kind::SellerStock {
                stock::inventory::Turn {
                    connection,
                    namespace,
                    selected: input,
                    crypto,
                    clock,
                    uncertain,
                }
                .option_mapping(*binding, minimum, key, proof)
                .await
            } else {
                Err(OperationError::UnsupportedOperation)
            };
            let _ = reply.send(result);
        }),
        Job::Pool(binding, minimum, id, proof, reply) => Box::pin(async move {
            let result = if kind == Kind::SellerStock {
                stock::inventory::Turn {
                    connection,
                    namespace,
                    selected: input,
                    crypto,
                    clock,
                    uncertain,
                }
                .pool(*binding, minimum, id, proof)
                .await
            } else {
                Err(OperationError::UnsupportedOperation)
            };
            let _ = reply.send(result);
        }),
        Job::StockAuthority(selected, proof, reply) => Box::pin(async move {
            let result = if kind == Kind::SellerStock && !*uncertain {
                stock::authenticate(
                    connection, namespace, input, crypto, clock, &selected, &proof,
                )
                .await
                .map(|(receipt, _)| receipt)
            } else {
                Err(WorkspaceError::RecoveryRequired)
            };
            if result == Err(WorkspaceError::CommitUnknown) {
                *uncertain = true;
            }
            let _ = reply.send(result);
        }),
        job @ (Job::RecoverStockEnvelope(..)
        | Job::RecoverStockObservation(..)
        | Job::RecoverCapture(..)
        | Job::RecoverStockEvidence(..)
        | Job::RecoverStockUnissued(..)) => Box::pin(async move { job.reject_recovery() }),
        Job::CaptureDeliveryObservation(binding, proof, observation, reply) => {
            Box::pin(async move {
                let result = Journal {
                    connection,
                    namespace,
                    selected: input,
                    crypto,
                    clock,
                    uncertain,
                }
                .capture_delivery_observation(*binding, proof, *observation)
                .await;
                let _ = reply.send(result);
            })
        }
        Job::QueryDeliveryObservations(
            binding,
            operation,
            digest,
            recipient,
            request,
            proof,
            reply,
        ) => Box::pin(async move {
            let result = Journal {
                connection,
                namespace,
                selected: input,
                crypto,
                clock,
                uncertain,
            }
            .query_delivery_observations(*binding, operation, digest, recipient, request, proof)
            .await;
            let _ = reply.send(result);
        }),
        Job::PrepareDelivery(request, proof, candidate, reply) => Box::pin(async move {
            let result = Journal {
                connection,
                namespace,
                selected: input,
                crypto,
                clock,
                uncertain,
            }
            .prepare_delivery(*request, proof, candidate.map(|value| *value))
            .await;
            let _ = reply.send(result);
        }),
        Job::QueryDelivery(binding, operation, digest, recipient, request, proof, reply) => {
            Box::pin(async move {
                let result = Journal {
                    connection,
                    namespace,
                    selected: input,
                    crypto,
                    clock,
                    uncertain,
                }
                .query_delivery(*binding, operation, digest, recipient, request, proof)
                .await;
                let _ = reply.send(result);
            })
        }
        Job::Install(request, proof, reply) => Box::pin(async move {
            let result = Journal {
                connection,
                namespace,
                selected: input,
                crypto,
                clock,
                uncertain,
            }
            .install_chat(*request, proof)
            .await;
            let _ = reply.send(result);
        }),
        Job::Snapshot(binding, operation, digest, proof, reply) => Box::pin(async move {
            let result = Journal {
                connection,
                namespace,
                selected: input,
                crypto,
                clock,
                uncertain,
            }
            .query_snapshot(*binding, operation, digest, proof)
            .await;
            let _ = reply.send(result);
        }),
        Job::Claim(request, proof, reply) => Box::pin(async move {
            let result = Journal {
                connection,
                namespace,
                selected: input,
                crypto,
                clock,
                uncertain,
            }
            .claim_issuance(*request, proof)
            .await;
            let _ = reply.send(result);
        }),
        Job::Attempt(request, proof, reply) => Box::pin(async move {
            let result = Journal {
                connection,
                namespace,
                selected: input,
                crypto,
                clock,
                uncertain,
            }
            .query_issuance(*request, proof)
            .await;
            let _ = reply.send(result);
        }),
        Job::Capture(selected, proof, evidence, reply) => Box::pin(async move {
            let result = Journal {
                connection,
                namespace,
                selected: input,
                crypto,
                clock,
                uncertain,
            }
            .capture_issuance(*selected, proof, *evidence)
            .await;
            let _ = reply.send(result);
        }),
        Job::Captured(request, proof, reply) => Box::pin(async move {
            let result = Journal {
                connection,
                namespace,
                selected: input,
                crypto,
                clock,
                uncertain,
            }
            .query_captured(*request, proof)
            .await;
            let _ = reply.send(result);
        }),
        Job::BindPlan(binding, plan, proof, reply) => Box::pin(async move {
            let result = Journal {
                connection,
                namespace,
                selected: input,
                crypto,
                clock,
                uncertain,
            }
            .bind_plan(*binding, plan, proof)
            .await;
            let _ = reply.send(result);
        }),
        Job::SigningPlan(binding, operation_id, digest, proof, reply) => Box::pin(async move {
            let result = Journal {
                connection,
                namespace,
                selected: input,
                crypto,
                clock,
                uncertain,
            }
            .query_plan(*binding, operation_id, digest, proof)
            .await;
            let _ = reply.send(result);
        }),
        Job::Authenticate(selected, proof, reply) => Box::pin(async move {
            let result = if *uncertain {
                Err(WorkspaceError::RecoveryRequired)
            } else if kind == Kind::SellerStock {
                stock::authenticate(
                    connection, namespace, input, crypto, clock, &selected, &proof,
                )
                .await
                .map(|(_, binding)| binding)
            } else {
                authenticate(
                    connection, namespace, input, crypto, clock, &selected, &proof,
                )
                .await
                .map(|(_, binding)| binding)
            };
            if result == Err(WorkspaceError::CommitUnknown) {
                *uncertain = true;
            }
            let _ = reply.send(result);
        }),
        Job::Revoke(selected, proof, reply) => Box::pin(async move {
            let result = if *uncertain {
                Err(WorkspaceError::RecoveryRequired)
            } else if kind == Kind::SellerStock {
                stock::revoke(
                    connection, namespace, input, crypto, clock, &selected, &proof,
                )
                .await
            } else {
                revoke(
                    connection, namespace, input, crypto, clock, &selected, &proof,
                )
                .await
            };
            if result == Err(WorkspaceError::CommitUnknown) {
                *uncertain = true;
            }
            let _ = reply.send(result);
        }),
        Job::Admit(request, proof, reply) => Box::pin(async move {
            let result = Journal {
                connection,
                namespace,
                selected: input,
                crypto,
                clock,
                uncertain,
            }
            .admit(*request, proof)
            .await;
            let _ = reply.send(result);
        }),
        Job::Operation(binding, operation_id, digest, proof, reply) => Box::pin(async move {
            let result = Journal {
                connection,
                namespace,
                selected: input,
                crypto,
                clock,
                uncertain,
            }
            .query(*binding, operation_id, digest, proof)
            .await;
            let _ = reply.send(result);
        }),
    }
}
