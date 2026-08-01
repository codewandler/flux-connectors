op babelforce-toggle-recording-flag(id: String) -> Any
  description "Toggle a recording's flag"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/recordings/{id}/flag")
  response = http.request(method: "POST", url)
  return response
