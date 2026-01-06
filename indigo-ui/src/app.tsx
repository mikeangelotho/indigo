import { Router } from "@solidjs/router";
import { FileRoutes } from "@solidjs/start/router";
import { Suspense, ErrorBoundary } from "solid-js";
import Nav from "~/components/Nav";
import "./app.css";

export default function App() {
  return (
    <Router
      root={props => (
        <>
          <Nav />
          <ErrorBoundary fallback={(err) => <div class="p-8 text-center text-red-400">Something went wrong: {err.toString()}</div>}>
            <Suspense fallback={
              <div class="flex h-[calc(100vh-64px)] items-center justify-center bg-zinc-950">
                <div class="flex flex-col items-center gap-4">
                  <div class="w-10 h-10 border-4 border-indigo-500/20 border-t-indigo-500 rounded-full animate-spin"></div>
                  <span class="text-zinc-500 text-sm font-medium">Initializing...</span>
                </div>
              </div>
            }>
              {props.children}
            </Suspense>
          </ErrorBoundary>
        </>
      )}
    >
      <FileRoutes />
    </Router>
  );
}
