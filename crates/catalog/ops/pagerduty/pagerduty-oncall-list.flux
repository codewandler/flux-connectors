op pagerduty-oncall-list(limit: Number, offset: Number) -> Any
  description "List who is currently on call, as one entry per escalation policy, escalation level and user. An entry with no `schedule` is a user attached directly to an escalation level rather than through a rotation. Time filtering is not offered by this connector, so this answers `who is on call now`"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.pagerduty.com"
  url = fmt("{base}/oncalls")
  Accept = "application/vnd.pagerduty+json;version=2"
  response = http.request(headers: { Accept }, method: "GET", query: { limit, offset }, url)
  return response
