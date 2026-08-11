// CHOROGRAPH-ANCHOR: shared/stub declarations only. Merged estate-wide by chorograph --anchors.
//
// This file lets `npx chorograph render .` pass inside this repo alone. When the whole
// Tatch-AI estate is rendered as one map, every repo's anchor.ts is deleted and replaced by a
// single master anchor, so nothing here may declare a node this repo actually owns — owned
// nodes live in chorograph/architecture.ts (and, in TS repos, inline on the code).

/**
 * Harper is an AI-forward commercial insurance brokerage. Revenue is commission on placed
 * premium; the platform runs the funnel from lead acquisition through intake, quoting and
 * placement, binding, payment, post-bind servicing, and renewal.
 * @system Harper
 */

/**
 * Infrastructure, DevOps, workflow orchestration, and platform tooling that supports the
 * estate but does not directly touch brokerage workflows or customer data.
 * @domain Platform
 */

/**
 * Harper's workflow orchestration engine. Hercules, relay-server, sluice, and other services
 * run durable workflows here; tempurview is a read-mostly operator console on top of it.
 * @external Temporal in:Platform
 */
