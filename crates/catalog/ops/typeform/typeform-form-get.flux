op typeform-form-get(form_id: String) -> Any
  description "Get one form's own definition: its title, language, rendering type and questions (fields), plus its welcome and thank-you screens. Does not return any response data — use typeform-response-list for that. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/description`, its error code at `/code` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.typeform.com"
  url = fmt("{base}/forms/{form_id}")
  response = http.request(method: "GET", url)
  return response
