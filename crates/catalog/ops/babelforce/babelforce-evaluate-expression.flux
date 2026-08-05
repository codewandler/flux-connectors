op babelforce-evaluate-expression(async: Bool, body: Any) -> Any
  description "Evaluates a single Expression based on a provided Context"
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/expressions/evaluate")
  content_type = "application/json"
  payload = parse(body, as: "json")
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", query: { async }, url)
  return response
