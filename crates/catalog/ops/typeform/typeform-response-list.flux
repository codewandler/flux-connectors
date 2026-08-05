op typeform-response-list(form_id: String, page_size: Number, before: String, after: String, since: String, until: String, response_type: String, sort: String) -> Any
  description "List one form's responses, newest first by default. Paginate with `before`/`after`, each naming the `token` of a response already retrieved, to page toward older or newer responses respectively; `page_size` alone returns only the first page. Each response's `answers` holds whatever that respondent typed or selected — this connector never inspects or filters on answer content. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/description`, its error code at `/code` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.typeform.com"
  url = fmt("{base}/forms/{form_id}/responses")
  response = http.request(method: "GET", query: { after, before, page_size, response_type, since, sort, until: $until }, url)
  return response
