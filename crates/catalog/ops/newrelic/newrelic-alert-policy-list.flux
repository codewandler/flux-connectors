op newrelic-alert-policy-list -> Any
  description "List the account's alert policies — the groupings New Relic evaluates conditions under, each with its incident rollup preference. Takes no argument. This lists the policies themselves, not the conditions inside them and not anything currently alerting; for that, read newrelic-alert-violation-list"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://{host}/v2"
  url = fmt("{base}/alerts_policies.json")
  response = http.request(method: "GET", url)
  return response
