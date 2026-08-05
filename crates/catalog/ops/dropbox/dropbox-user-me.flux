op dropbox-user-me -> Any
  description "Get the account this access token authenticates as, confirming the token resolves and naming the account it belongs to. Takes no parameters. Dropbox routes this read through POST, like every operation this connector declares — there is no GET anywhere in Dropbox's v2 API. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error_summary` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.dropboxapi.com"
  url = fmt("{base}/2/users/get_current_account")
  response = http.request(method: "POST", url)
  return response
