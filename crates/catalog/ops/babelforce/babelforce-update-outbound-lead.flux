op babelforce-update-outbound-lead(id: String, leadId: String, body: Any) -> Any
  description "Update a lead's meta-data"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/outbound/lists/{id}/leads/{leadId}")
  content_type = "application/json"
  payload = parse(body, as: "json")
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PUT", url)
  return response
