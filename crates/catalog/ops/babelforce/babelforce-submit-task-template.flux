op babelforce-submit-task-template(template: String, style: String, body: Any) -> Any
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/tasks/template/{template}")
  sep = "?"
  when style
    url = fmt("{url}{sep}style={style}")
  content_type = "application/json"
  payload = parse(body, as: "json")
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
