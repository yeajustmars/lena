# Module `main`

**Source**: `lena-adapter-go/tests/fixtures/go/gin/gin/1.9.1/gin.go`

## `Unnamed`

## `Unnamed`

## `Unnamed`

## `Unnamed`

## `StandaloneFunc`

StandaloneFunc is a standard function with a docstring.

## `getFixturePath`

## `hashId`

## `PointerReceiver`

PointerReceiver method

## `countDbAdds`

## `ValueReceiver`

ValueReceiver method

## `escapeString`

## `TestGoEdgeCasesParse`

## `TestGoModuleEntityCreation`

## `getFuncPrefix`

getFuncPrefix resolves the correct prefix (Package or Package.Struct) for a function

## `Last`

Last returns the last handler in the chain. i.e. the last handler is the main one.

## `TestGoMethodReceiverCorrelation`

## `NewSymbolTable`

## `TestGoDocstringExtraction`

## `TestGoHashIdMasking`

## `discoverSymbols`

discoverSymbols (Pass 1) harvests imports and function declarations into the SymbolTable

## `TestGoEmptyFileGracefulHandling`

## `emitFacts`

emitFacts (Pass 2) walks the AST again, generating module, function, and call-site edges

## `TestGoMalformedSyntaxResilience`

## `TestGoFunctionPositions`

## `TestGoMultipleFunctions`

## `TestGoGinStressAddCount`

## `TestGoCallSiteExtraction`

## `New`

New returns a new blank Engine instance without any middleware attached.
By default, the configuration is:
- RedirectTrailingSlash:  true
- RedirectFixedPath:      false
- HandleMethodNotAllowed: false
- ForwardedByClientIP:    true
- UseRawPath:             false
- UnescapePathValues:     true

## `RunTwoPass`

RunTwoPass orchestrates the two-pass extraction logic over a slice of file paths

## `main`

## `Default`

Default returns an Engine instance with the Logger and Recovery middleware already attached.

## `Handler`

## `allocateContext`

## `Delims`

Delims sets template left and right delims and returns an Engine instance.

## `SecureJsonPrefix`

SecureJsonPrefix sets the secureJSONPrefix used in Context.SecureJSON.

## `LoadHTMLGlob`

LoadHTMLGlob loads HTML files identified by glob pattern
and associates the result with HTML renderer.

## `LoadHTMLFiles`

LoadHTMLFiles loads a slice of HTML files
and associates the result with HTML renderer.

## `SetHTMLTemplate`

SetHTMLTemplate associate a template with HTML renderer.

## `SetFuncMap`

SetFuncMap sets the FuncMap used for template.FuncMap.

## `NoRoute`

NoRoute adds handlers for NoRoute. It returns a 404 code by default.

## `NoMethod`

NoMethod sets the handlers called when Engine.HandleMethodNotAllowed = true.

## `Use`

Use attaches a global middleware to the router. i.e. the middleware attached through Use() will be
included in the handlers chain for every single request. Even 404, 405, static files...
For example, this is the right place for a logger or error management middleware.

## `rebuild404Handlers`

## `rebuild405Handlers`

## `addRoute`

## `Routes`

Routes returns a slice of registered routes, including some useful information, such as:
the http method, path and the handler name.

## `iterate`

## `Run`

Run attaches the router to a http.Server and starts listening and serving HTTP requests.
It is a shortcut for http.ListenAndServe(addr, router)
Note: this method will block the calling goroutine indefinitely unless an error happens.

## `prepareTrustedCIDRs`

## `SetTrustedProxies`

SetTrustedProxies set a list of network origins (IPv4 addresses,
IPv4 CIDRs, IPv6 addresses or IPv6 CIDRs) from which to trust
request's headers that contain alternative client IP when
`(*gin.Engine).ForwardedByClientIP` is `true`. `TrustedProxies`
feature is enabled by default, and it also trusts all proxies
by default. If you want to disable this feature, use
Engine.SetTrustedProxies(nil), then Context.ClientIP() will
return the remote address directly.

## `isUnsafeTrustedProxies`

isUnsafeTrustedProxies checks if Engine.trustedCIDRs contains all IPs, it's not safe if it has (returns true)

## `parseTrustedProxies`

parseTrustedProxies parse Engine.trustedProxies to Engine.trustedCIDRs

## `isTrustedProxy`

isTrustedProxy will check whether the IP address is included in the trusted list according to Engine.trustedCIDRs

## `validateHeader`

validateHeader will parse X-Forwarded-For header and return the trusted client IP address

## `parseIP`

parseIP parse a string representation of an IP and returns a net.IP with the
minimum byte representation or nil if input is invalid.

## `RunTLS`

RunTLS attaches the router to a http.Server and starts listening and serving HTTPS (secure) requests.
It is a shortcut for http.ListenAndServeTLS(addr, certFile, keyFile, router)
Note: this method will block the calling goroutine indefinitely unless an error happens.

## `RunUnix`

RunUnix attaches the router to a http.Server and starts listening and serving HTTP requests
through the specified unix socket (i.e. a file).
Note: this method will block the calling goroutine indefinitely unless an error happens.

## `RunFd`

RunFd attaches the router to a http.Server and starts listening and serving HTTP requests
through the specified file descriptor.
Note: this method will block the calling goroutine indefinitely unless an error happens.

## `RunListener`

RunListener attaches the router to a http.Server and starts listening and serving HTTP requests
through the specified net.Listener

## `ServeHTTP`

ServeHTTP conforms to the http.Handler interface.

## `HandleContext`

HandleContext re-enters a context that has been rewritten.
This can be done by setting c.Request.URL.Path to your new target.
Disclaimer: You can loop yourself to deal with this, use wisely.

## `handleHTTPRequest`

## `serveError`

## `redirectTrailingSlash`

## `redirectFixedPath`

## `redirectRequest`

