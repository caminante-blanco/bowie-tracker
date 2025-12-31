use leptos::*;

fn main() {
    console_error_panic_hook::set_once();
    tracing_wasm::set_as_global_default();
    mount_to_body(|| view! { <App/> })
}

#[component]
fn App() -> impl IntoView {
    view! {
        <div class="app-container">
            <header>
                <h1>"Bowie Tracker"</h1>
            </header>
            <main>
                <p>"Loading..."</p>
            </main>
        </div>
    }
}