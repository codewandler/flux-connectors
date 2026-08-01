op babelforce-bulk-update-applications(items: List<Any>) -> Any
  description "Update multiple applications"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/applications/bulk")
  content_type = "application/json"
  payload = { items }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PUT", url)
  return response
