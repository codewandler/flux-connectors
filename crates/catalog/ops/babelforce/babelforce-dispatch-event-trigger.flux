op babelforce-dispatch-event-trigger(eventTriggerId: String, timeout: Number, simulateCall: Bool, body: Any) -> Any
  description "Dispatch an event trigger"
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/api/v2/events/triggers/{eventTriggerId}/dispatch")
  sep = "?"
  when $timeout
    url = fmt("{url}{sep}timeout={timeout}")
    sep = "&"
  when simulateCall
    url = fmt("{url}{sep}simulateCall={simulateCall}")
  content_type = "application/json"
  payload = parse(body, as: "json")
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
