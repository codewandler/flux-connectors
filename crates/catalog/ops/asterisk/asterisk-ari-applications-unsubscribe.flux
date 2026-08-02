op asterisk-ari-applications-unsubscribe(applicationName: String, eventSource: List<String>) -> Any
  description "Unsubscribe an application from an event source."
  risk "destructive"
  idempotency "non_idempotent"
  effects ["network"]
  expose false

  base = "https://{host}:8089/ari"
  url = fmt("{base}/applications/{applicationName}/subscription?eventSource={eventSource}")
  response = http.request(method: "DELETE", url)
  return response
