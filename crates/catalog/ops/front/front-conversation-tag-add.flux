op front-conversation-tag-add(conversation_id: String, tag_ids: List<String>) -> Any
  description "Apply one or more existing tags to a conversation. Applying a tag the conversation already carries changes nothing — Front does not duplicate it. Answers 204 with an empty body on success. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/_error/message`, its error code at `/_error/status` in the response body."
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api2.frontapp.com"
  url = fmt("{base}/conversations/{conversation_id}/tags")
  content_type = "application/json"
  payload = { tag_ids }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
