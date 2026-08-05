op typeform-form-list(page: Number, page_size: Number, sort_by: String, order_by: String) -> Any
  description "List the forms in the authenticated account, most recently updated forms sorting available. Returns each form's id, title and public/private status but not its questions — use typeform-form-get for a form's fields. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/description`, its error code at `/code` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.typeform.com"
  url = fmt("{base}/forms")
  response = http.request(method: "GET", query: { order_by, page, page_size, sort_by }, url)
  return response
