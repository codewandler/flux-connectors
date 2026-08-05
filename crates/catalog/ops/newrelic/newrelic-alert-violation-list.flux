op newrelic-alert-violation-list -> Any
  description "List the account's alert violations from New Relic's default recent window. READ `closed_at` ON EVERY ENTRY: this returns closed violations as well as open ones, and a violation with a non-null `closed_at` has already resolved. Each entry names the policy and condition that fired and the entity that broke"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{host}/v2"
  url = fmt("{base}/alerts_violations.json")
  response = http.request(method: "GET", url)
  return response
