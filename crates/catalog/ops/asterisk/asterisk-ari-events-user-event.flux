op asterisk-ari-events-user-event(eventName: String, application: String, source: List<String>, body: Any) -> Any
  description "Generate a user event."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/events/user/{eventName}?application={application}")
  sep = "&"
  when source
    url = fmt("{url}{sep}source={source}")
  content_type = "application/json"
  payload = parse(body, as: "json")
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
