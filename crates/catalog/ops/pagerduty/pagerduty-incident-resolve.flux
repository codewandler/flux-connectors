op pagerduty-incident-resolve(id: String, from_email: String) -> Any
  description "Resolve one incident, on behalf of a named PagerDuty user. This closes it: it leaves the open-incident view and notifications stop. If the underlying condition is still live, the next alert opens a brand-new incident rather than reopening this one, so resolve only what is actually fixed"
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://api.pagerduty.com"
  url = fmt("{base}/incidents/{id}")
  content_type = "application/json"
  Accept = "application/vnd.pagerduty+json;version=2"
  incident_type = "incident_reference"
  status = "resolved"
  payload = { incident: { status, type: incident_type } }
  response = http.request(body: payload, headers: { Accept, From: from_email, "content-type": content_type }, method: "PUT", url)
  return response
