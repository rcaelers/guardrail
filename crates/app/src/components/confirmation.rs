use leptos::*;

#[allow(non_snake_case)]
#[component]
pub fn ConfirmationModal(
    show: ReadSignal<bool>,
    custom_text: ReadSignal<String>,
    on_yes_click: Callback<()>,
    on_no_click: Callback<()>,
) -> impl IntoView {
    view! {
        {move || {
            if show.get() {
                view! {
                    <div class="fixed inset-0 flex items-center justify-center bg-gray-900 bg-opacity-50">
                        <div class="modal modal-open">
                            <div class="modal-box">
                                <h2 class="font-bold text-lg">{custom_text.get()}</h2>
                                <h3 class="mt-2">"Are you sure?"</h3>
                                <div class="modal-action">
                                    <button class="btn" on:click=move |_| on_no_click(())>
                                        "No"
                                    </button>
                                    <button
                                        class="btn btn-primary"
                                        on:click=move |_| on_yes_click(())
                                    >
                                        "Yes"
                                    </button>
                                </div>
                            </div>
                        </div>
                    </div>
                }
                    .into_view()
            } else {
                view! {}.into_view()
            }
        }}
    }
}
