op babelforce-authorize(response_type: String, client_id: String, redirect_uri: String, scope: String, state: String, code_challenge: String, code_challenge_method: String) -> Any
  description "OAuth 2.0 authorization endpoint"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose false

  base = "https://services.babelforce.com"
  url = fmt("{base}/oauth/authorize?response_type={response_type}&client_id={client_id}&redirect_uri={redirect_uri}&scope={scope}")
  sep = "&"
  when state
    url = fmt("{url}{sep}state={state}")
    sep = "&"
  when code_challenge
    url = fmt("{url}{sep}code_challenge={code_challenge}")
    sep = "&"
  when code_challenge_method
    url = fmt("{url}{sep}code_challenge_method={code_challenge_method}")
  response = http.request(method: "GET", url)
  return response
