op pagerduty-service-list(limit: Number, offset: Number) -> Any
  description "List the services this key can see. A service is what incidents are opened against and what an escalation policy is attached to, so this is how to discover the ids and names behind an incident's `service` reference. Also this connector's connection test"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.pagerduty.com"
  url = fmt("{base}/services")
  sep = "?"
  when limit
    url = fmt("{url}{sep}limit={limit}")
    sep = "&"
  when offset
    url = fmt("{url}{sep}offset={offset}")
  Accept = "application/vnd.pagerduty+json;version=2"
  response = http.request(headers: { Accept }, method: "GET", url)
  return response
