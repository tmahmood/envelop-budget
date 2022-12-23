mod imp;

use crate::new_transaction_dialog::NewTransactionDialog;
use crate::transaction::transaction_object::TransactionObject;
use crate::transaction::transaction_row::TransactionRow;
use std::borrow::{Borrow, BorrowMut};
use std::fmt::Error;
use std::num::ParseFloatError;
use std::ops::Deref;

use adw::glib::{closure_local, BindingFlags};

use crate::category::category_object::CategoryObject;
use crate::category::category_row::CategoryRow;
use crate::fix_float;
use crate::summary::summary_object::imp::SummaryData;
use crate::summary::summary_object::SummaryObject;
use adw::builders::ToastBuilder;
use adw::prelude::*;
use adw::Application;
use budget_manager::budgeting::budgeting_errors::BudgetingErrors;
use budget_manager::budgeting::Budgeting;
use budget_manager::DEFAULT_CATEGORY;
use gtk::glib::{clone, Object};
use gtk::subclass::prelude::*;
use gtk::{
    gio, glib, Entry, ListBox, ListBoxRow, NoSelection, ResponseType, StringList, ToggleButton,
};
use rand::distributions::uniform::SampleBorrow;

glib::wrapper! {
pub struct Window(ObjectSubclass<imp::Window>)
    @extends adw::ApplicationWindow, gtk::Window, gtk::Widget,
    @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible, gtk::Buildable,
                gtk::ConstraintTarget, gtk::Native, gtk::Root, gtk::ShortcutManager;
}

impl Window {
    pub fn new(app: &Application) -> Self {
        Object::builder().property("application", app).build()
    }

    pub fn setup_budget_account(&self) {
        self.imp().current_category_id.replace(1);
        let mut conn = budget_manager::establish_connection();
        budget_manager::run_migrations(&mut conn).expect("Failed to initialize database");
        let mut budgeting = Budgeting::new();
        budgeting
            .set_current_budget("main")
            .or_else(|_| budgeting.new_budget("main", 0.))
            .expect("Failed to get budget account");
        self.imp().budgeting.replace(budgeting);
    }

    fn setup_transactions(&self) {
        let mut budgeting = self.imp().budgeting.borrow_mut();
        let model = gio::ListStore::new(TransactionObject::static_type());
        let cid = self.imp().current_category_id.borrow();
        let mut category = budgeting
            .get_category_model_by_id(cid.deref().clone())
            .unwrap();

        category.transactions().iter().for_each(|transaction| {
            let mut tm = budgeting.transaction_model(transaction.clone());
            let transaction_object = TransactionObject::new(&mut tm);
            model.append(&transaction_object);
        });

        self.imp().transactions.replace(Some(model));
        let selection_model = NoSelection::new(Some(&self.transactions()));
        self.imp().transactions_list.bind_model(
            Some(&selection_model),
            clone!(@weak self as window => @default-panic, move |obj| {
                let transaction_obj = obj.downcast_ref().expect("The object should be of type `TransactionObject`.");
                let row = window.create_transaction_row(transaction_obj);
                row.upcast()
            }),
        );
        let transactions = self.transactions();
        self.set_transactions_list_visible_only_when_there_are_transactions(&transactions);
    }

    fn setup_categories(&self) {
        let mut budgeting = self.imp().budgeting.borrow_mut();
        let model = gio::ListStore::new(CategoryObject::static_type());
        budgeting.all_categories().iter().for_each(|category| {
            let mut cm = budgeting.category_model(category.clone());
            let category_object = CategoryObject::new(&mut cm);
            model.append(&category_object);
        });

        self.imp().categories.replace(Some(model));
        let selection_model = NoSelection::new(Some(&self.categories()));
        self.imp().categories_list.bind_model(
            Some(&selection_model),
            clone!(@weak self as window => @default-panic, move |obj| {
                let category_obj = obj.downcast_ref().expect("The object should be of type `CategoryObject`.");
                let row = window.create_category_row(category_obj);
                row.upcast()
            }),
        );
        let categories = self.categories();
        self.set_categories_list_visible_only_when_there_are_categories(&categories);
    }

    pub(crate) fn update_budget_details(&self) {
        let mut budgeting = self.imp().budgeting.borrow_mut();
        let cid = self.imp().current_category_id.borrow();
        let mut category = budgeting
            .get_category_model_by_id(cid.deref().clone())
            .unwrap();

        let expense = category.expense();
        let _transfer_out = category.transfer_out();
        let total_expense = fix_float(expense * expense.signum());
        let total_income = fix_float(category.income());
        let transfer_in = fix_float(category.transfer_in());
        let transfer_out = fix_float(_transfer_out * _transfer_out.signum());

        let b = category.balance();
        let balance = fix_float(b);
        let category_name = category.category().name();

        let heading = self.imp().transaction_title.get();
        heading.set_title(&category_name);

        if category_name == DEFAULT_CATEGORY || b >= category.allocated() {
            self.imp().fund_overspent.set_sensitive(false);
        } else {
            self.imp().fund_overspent.set_sensitive(true);
        }

        let summary_table = self.imp().summary_table.borrow().get();
        if b < 0. {
            heading.add_css_class("error");
            summary_table.add_css_class("error");
        } else {
            heading.remove_css_class("error");
            summary_table.remove_css_class("error");
        }
        self.imp().summary_table.imp().toggle.set_label(&balance);
        let summary_data = SummaryData {
            transfer_in,
            transfer_out,
            total_income,
            total_expense,
        };
        let summary_object = SummaryObject::new(summary_data);
        summary_table.bind_summary(&summary_object);
    }

    pub(crate) fn transactions(&self) -> gio::ListStore {
        self.imp().transactions.borrow().clone().unwrap()
    }

    fn categories(&self) -> gio::ListStore {
        self.imp().categories.borrow().clone().unwrap()
    }

    pub(crate) fn current_category_id(&self) -> i32 {
        *self.imp().current_category_id.borrow().deref()
    }

    fn set_transactions_list_visible_only_when_there_are_transactions(
        &self,
        transactions: &gio::ListStore,
    ) {
        self.imp()
            .transactions_list
            .set_visible(transactions.n_items() > 0);
    }

    fn set_categories_list_visible_only_when_there_are_categories(
        &self,
        categories: &gio::ListStore,
    ) {
        self.imp()
            .categories_list
            .set_visible(categories.n_items() > 0);
    }

    fn create_category_row(&self, category_object: &CategoryObject) -> CategoryRow {
        CategoryRow::new().bind_objects(category_object)
    }

    fn create_transaction_row(&self, transaction_object: &TransactionObject) -> TransactionRow {
        TransactionRow::new().bind_objects(transaction_object)
    }

    fn setup_actions(&self) {
        // Create action to create new collection and add to action group "win"
        let action_new_list = gio::SimpleAction::new("new-transaction", None);
        action_new_list.connect_activate(clone!(@weak self as window => move |_, _| {
            window.new_transaction();
        }));
        self.add_action(&action_new_list);

        let action_fund_transfer = gio::SimpleAction::new("fund-transfer", None);
        action_fund_transfer.connect_activate(clone!(@weak self as window => move |_, _| {
            window.fund_transfer();
        }));
        self.add_action(&action_fund_transfer);
    }

    fn fund_transfer(&self) {
        let mut budget_account = self.imp().budgeting.borrow_mut();
        let cid = self.imp().current_category_id.borrow();
        let mut category = budget_account
            .get_category_model_by_id(cid.deref().clone())
            .unwrap();
        let category_name = category.category().name();

        match budget_account.fund_from_unallocated(&category_name) {
            Ok(_) => {}
            Err(BudgetingErrors::AlreadyFunded) => self.show_toast("No need to fund"),
            Err(BudgetingErrors::OverFundingError) => self.show_toast("You do not have enough money to fund this category"),
            Err(e) => { self.show_toast(&format!("Something went wrong. {:?}", e)) }
        }
    }

    fn new_transaction(&self) {
        // Create new Dialog
        let mut b = self.imp().budgeting.borrow_mut();
        let categories = b.categories();
        let dialog = NewTransactionDialog::new(self, categories);
        dialog.connect_closure(
            "valid-transaction-entered",
            false,
            closure_local!(@watch self as window => move |dialog: NewTransactionDialog| {
                let entry_payee = dialog.imp().entry_payee.get();
                let entry_note = dialog.imp().entry_note.get();
                let entry_amount = dialog.imp().entry_amount.get();
                let toggle_income = dialog.imp().toggle_income.get();
                let entry_date = dialog.imp().transaction_date.get();
                let category_name = dialog.imp().category_selected.borrow();

                let payee = entry_payee.text();
                let note = entry_note.text();
                let amount = entry_amount.value();
                let date = entry_date.imp().date().unwrap();

                dialog.destroy();
                let category_id = {
                    let mut budgeting = window.imp().budgeting.borrow_mut();
                    let mut tb = budgeting.new_transaction_to_category(&category_name);
                    if toggle_income.is_active() {
                        tb.income(amount);
                    } else {
                        tb.expense(amount);
                    }
                    let t = tb.payee(&payee).date_created(date).note(&note).done();
                    t.category_id()
                };
                if window.current_category_id() == category_id {
                    window.update_budget_details();
                    window.setup_transactions();
                }
            }),
        );
        dialog.present();
    }

    fn setup_callbacks(&self) {
        self.imp()
            .back_button
            .connect_clicked(clone!(@weak self as window => move |_| {
                window.imp().leaflet.navigate(adw::NavigationDirection::Back);
            }));

        self.transactions().connect_items_changed(
            clone!(@weak self as window => move |transactions, _, _, _| {
                window.set_transactions_list_visible_only_when_there_are_transactions(transactions);
            }),
        );

        self.categories().connect_items_changed(
            clone!(@weak self as window => move |categories, _, _, _| {
                window.set_categories_list_visible_only_when_there_are_categories(categories);
            }),
        );

        self.imp()
            .fund_overspent
            .connect_clicked(clone!(@weak self as window => move |_| {
                // will try to allocate money to this category
                window.imp().leaflet.navigate(adw::NavigationDirection::Back);
            }));
    }

    fn show_toast(&self, text: &str) {
        let t = self.imp().toast_overlay.get();
        let toast = ToastBuilder::new().title(text).build();
        t.add_toast(&toast);
        t.show();
    }
}
