// SPDX-License-Identifier: AGPL-3.0-only

use libsql::Value;

pub fn values_from_strings(values: &[String]) -> Vec<Value> {
    values.iter().cloned().map(Value::Text).collect()
}
