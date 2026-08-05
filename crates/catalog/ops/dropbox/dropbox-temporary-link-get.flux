op dropbox-temporary-link-get(path: String) -> Any
  description "Get a temporary, unauthenticated download link for an existing file's content, valid for approximately four hours. This connector cannot follow the link itself — it only returns the URL, for a caller to fetch separately. Direction remains conservatively authored as write pending individual review; POST is transport only and supplied no evidence. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error_summary` in the response body."
  risk "medium"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://api.dropboxapi.com"
  url = fmt("{base}/2/files/get_temporary_link")
  content_type = "application/json"
  payload = { path }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
