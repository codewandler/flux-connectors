op babelforce-update-settings-for-audit-default(body: Any) -> Any
  description "Update default settings"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/settings/audit/default")
  content_type = "application/json"
  payload = parse(body, as: "json")
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PUT", url)
  return response
