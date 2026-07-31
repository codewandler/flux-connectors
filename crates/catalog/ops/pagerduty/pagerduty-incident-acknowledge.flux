op pagerduty-incident-acknowledge(id: String, from_email: String) -> Any
  description "Acknowledge one incident, on behalf of a named PagerDuty user. This stops the escalation clock and tells the rotation that somebody is working the incident; it does not close it. PagerDuty refuses the change if the incident is already resolved, and answers that as data rather than as a failure"
  risk "medium"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.pagerduty.com"
  url = fmt("{base}/incidents/{id}")
  content_type = "application/json"
  Accept = "application/vnd.pagerduty+json;version=2"
  incident_type = "incident_reference"
  status = "acknowledged"
  payload = { incident: { status, type: incident_type } }
  response = http.request(body: payload, headers: { Accept, From: from_email, "content-type": content_type }, method: "PUT", url)
  return response
