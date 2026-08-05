op babelforce-bulk-update-integrations(items: List<Any>) -> Any
  description "Bulk-update integrations"
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/integrations/bulk")
  content_type = "application/json"
  payload = { items }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PUT", url)
  return response
