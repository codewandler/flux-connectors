op statuspage-incident-update(incident_id: String, status: String, body: String, deliver_notifications: Bool) -> Any
  description "Post an update to an existing incident on this status page, moving it to a new lifecycle stage and adding a publicly visible message. The update appears on the page immediately, and with deliver_notifications true Statuspage emails and texts every subscriber. The incident's status can be moved again afterwards, but a notification that has been sent cannot be recalled. Does not rename the incident — this operation cannot change its title. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error` in the response body."
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://api.statuspage.io/v1/pages/{page_id}"
  url = fmt("{base}/incidents/{incident_id}")
  content_type = "application/json"
  payload = { incident: { body, deliver_notifications, status } }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PATCH", url)
  return response
