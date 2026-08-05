op intercom-conversation-get(conversation_id: String) -> Any
  description "Get one conversation by id, with its state, the contacts on it and its message parts in Intercom's default rendering. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/errors/0/message`, its error code at `/errors/0/code` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://{host}"
  url = fmt("{base}/conversations/{conversation_id}")
  response = http.request(method: "GET", url)
  return response
