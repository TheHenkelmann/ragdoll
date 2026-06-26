# Product FAQ - Lumen Insights Platform

Frequently asked questions from our customers about the Lumen Insights Platform,
the SaaS product of Lumen Analytics GmbH. This document is public and may be
shared externally. For a machine-readable summary of plan limits, see our public
pricing list.

## What pricing plans are available?

The platform is offered in three plans: **Starter**, **Team**, and
**Enterprise**. The Starter plan suits small projects and costs 49 euros per
month. The Team plan costs 199 euros per month and includes more data volume as
well as shared dashboards. The Enterprise plan is priced individually and adds
single sign-on, a dedicated support team, and a guaranteed SLA. All plans are
billed monthly, and annual billing comes with a two-month discount.

You can upgrade or downgrade at any time. When you upgrade, the change takes
effect immediately and the cost is prorated for the rest of the billing period.
When you downgrade, the change applies at the start of the next billing period
so that you keep the higher limits you already paid for.

## How long is my data retained?

Raw data is retained for 30 days on the Starter plan, 180 days on the Team plan,
and up to 24 months on the Enterprise plan. Aggregated metrics are kept
indefinitely as long as the account is active. On request, data can be deleted
completely through the API or via support; deletion is completed within 14 days.

When an account is cancelled, raw data is deleted after a 30-day grace period,
during which you can still export everything. You can request a full export of
your data at any time in standard formats such as CSV and JSON, so you are never
locked in.

## Which integrations are supported?

Lumen Insights connects to common data sources, including PostgreSQL, Snowflake,
BigQuery, and CSV uploads. For automation there is a REST API with API keys.
Webhooks notify external systems about completed analyses. A native integration
with popular BI tools is available from the Team plan upward.

We also provide official client libraries for Python and JavaScript, and a
command-line tool for scripted exports. Rate limits depend on the plan: the
Starter plan allows 60 API requests per minute, while the Team and Enterprise
plans allow significantly higher throughput that can be tuned on request.

## What availability is guaranteed?

On the Enterprise plan we guarantee a monthly availability of 99.9 percent.
Planned maintenance windows are announced at least 72 hours in advance and fall
outside Central European business hours. If the SLA is missed, we grant service
credits according to the contract.

Our public status page shows real-time availability and the history of past
incidents. We follow a transparent communication policy: during a major
incident, updates are posted at least every 30 minutes until the issue is
resolved.

## How do I reach support?

Support is available by email on all plans. The Team plan additionally includes
chat support during business hours, and the Enterprise plan includes 24/7
emergency phone support. The average first response time on the Team plan is
under four hours.

Beyond direct support, we maintain extensive documentation, a community forum,
and a library of video tutorials. Enterprise customers also receive a named
customer-success manager who runs quarterly business reviews and helps plan
upcoming usage.

## How is my data kept secure?

Data is encrypted in transit and at rest. Access is protected by single sign-on
on the Enterprise plan, and all customer data is logically isolated per account.
For details on our internal controls, security whitepapers are available under a
non-disclosure agreement for Enterprise prospects.
