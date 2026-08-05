op typeform-response-delete(form_id: String, included_response_ids: String) -> Any
  description "Permanently delete one or more of a form's responses by their own token. There is no undelete: once deleted, a response's answers are gone. Deletion is asynchronous — a successful call confirms the request was registered, not that the responses are already gone. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/description`, its error code at `/code` in the response body."
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://api.typeform.com"
  url = fmt("{base}/forms/{form_id}/responses")
  response = http.request(method: "DELETE", query: { included_response_ids }, url)
  return response
