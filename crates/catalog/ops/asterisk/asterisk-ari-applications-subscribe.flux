op asterisk-ari-applications-subscribe(applicationName: String, eventSource: List<String>) -> Any
  description "Subscribe an application to a event source."
  risk "high"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/applications/{applicationName}/subscription?eventSource={eventSource}")
  response = http.request(method: "POST", url)
  return response
