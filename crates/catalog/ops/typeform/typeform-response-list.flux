op typeform-response-list(form_id: String, page_size: Number, before: String, after: String, since: String, until: String, response_type: String, sort: String) -> Any
  description "List one form's responses, newest first by default. Paginate with `before`/`after`, each naming the `token` of a response already retrieved, to page toward older or newer responses respectively; `page_size` alone returns only the first page. Each response's `answers` holds whatever that respondent typed or selected — this connector never inspects or filters on answer content. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/description`, its error code at `/code` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.typeform.com"
  url = fmt("{base}/forms/{form_id}/responses")
  sep = "?"
  when page_size
    url = fmt("{url}{sep}page_size={page_size}")
    sep = "&"
  when before
    url = fmt("{url}{sep}before={before}")
    sep = "&"
  when after
    url = fmt("{url}{sep}after={after}")
    sep = "&"
  when since
    url = fmt("{url}{sep}since={since}")
    sep = "&"
  when $until
    url = fmt("{url}{sep}until={until}")
    sep = "&"
  when response_type
    url = fmt("{url}{sep}response_type={response_type}")
    sep = "&"
  when sort
    url = fmt("{url}{sep}sort={sort}")
  response = http.request(method: "GET", url)
  return response
