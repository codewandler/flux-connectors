op typeform-form-list(page: Number, page_size: Number, sort_by: String, order_by: String) -> Any
  description "List the forms in the authenticated account, most recently updated forms sorting available. Returns each form's id, title and public/private status but not its questions — use typeform-form-get for a form's fields. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/description`, its error code at `/code` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.typeform.com"
  url = fmt("{base}/forms")
  sep = "?"
  when page
    url = fmt("{url}{sep}page={page}")
    sep = "&"
  when page_size
    url = fmt("{url}{sep}page_size={page_size}")
    sep = "&"
  when sort_by
    url = fmt("{url}{sep}sort_by={sort_by}")
    sep = "&"
  when order_by
    url = fmt("{url}{sep}order_by={order_by}")
  response = http.request(method: "GET", url)
  return response
