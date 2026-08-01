op babelforce-update-script(codeId: String, type: String, file: String, metadata: Any) -> Any
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/tasks/scripts/{type}/{codeId}")
  content_type = "application/json"
  payload = { file, metadata }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "PUT", url)
  return response
