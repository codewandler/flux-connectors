op babelforce-create-selection-configuration(accept_timeout: Number, complete_timeout: Number, expire_in: Number, reschedule_delay: Number, selection_engine: String) -> Any
  risk "high"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v3/tasks/configurations/selection")
  content_type = "application/json"
  payload = { accept_timeout, complete_timeout, expire_in, reschedule_delay, selection_engine }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
