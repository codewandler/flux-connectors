op babelforce-delete-secret-keys(prefix: String, body: List<String>) -> Any
  description "Delete a list of secrets"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/tasks/configurations/secrets/{prefix}")
  content_type = "application/json"
  payload = parse(body, as: "json")
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "DELETE", url)
  return response
