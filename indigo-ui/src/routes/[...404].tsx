import { A } from "@solidjs/router";

export default function NotFound() {
  return (
    <main class="max-w-2xl mx-auto p-8 text-center">
      <div class="ng-card p-10 inline-block">
        <h1 class="text-5xl font-bold text-white font-data tracking-widest mb-4">404</h1>
        <p class="text-ng-secondary mb-6 text-sm">Node not found in grid topology.</p>
        <A href="/" class="btn-primary inline-block">
          Return to Base
        </A>
      </div>
    </main>
  );
}
