op pagerduty-incident-get(id: String) -> Any
  description "Fetch one incident in full, including its current status, urgency, service, assignments and acknowledgements. Read this before acknowledging or resolving, because PagerDuty refuses a status change that does not follow from the incident's current status"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.pagerduty.com"
  url = fmt("{base}/incidents/{id}")
  Accept = "application/vnd.pagerduty+json;version=2"
  response = http.request(headers: { Accept }, method: "GET", url)
  return response
