op asterisk-ari-applications-filter(applicationName: String, body: Any) -> Any
  description "Filter application events types."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/applications/{applicationName}/eventFilter")
  content_type = "application/json"
  payload = parse(body, as: "json")
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PUT", url)
  return response
