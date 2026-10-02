# Mandatory Implementation Standards

## Frontend (React, Vite, Redux, Axios)
- **State Management:** Use ONLY Redux for global state. Do not create standalone Context APIs.
- **Requests:** Use the Axios instances configured in the project. Never use native `fetch`.
- **Components:** Create only functional components. Exports must be named (do not use `export default`).
- **Style Management:** Use TailwindCSS for styling. Do not use inline styles or separate CSS files.
- **Component Architecture:** Use Atomic Design for component architecture.
- **Testing:** Create tests for all components. Use React Testing Library for testing.
- **Type Definition:** Use TypeScript for type definition.
- **Redux Toolkit:** Use Redux Toolkit for state management. Create a slice for each feature.
- **Axios Interceptors:** Use Axios interceptors for request and response handling. For example, add the auth token to the requests and handle 401 errors to redirect to the login page.
- **Axios Error Handling:** Use Axios interceptors for error handling. For example, handle 401 errors to redirect to the login page.
- **Redux slices:** Call services and handle business logic. Don't call services in components.
- **Services:** Deal with axios backend requests
- **Routing:** Use React Router for routing.
- **Form Management:** Use React Hook Form for form management.
- **Form Validation:** Use Yup for form validation.
- **Form Submission:** Use React Hook Form for form submission.
- **Form Reset:** Use React Hook Form for form reset.
- **Form Clearing:** Use React Hook Form for form clearing.
- **Visual behaviors:** Every button in the app should have hover effects and disabled state styles, use the styles defined in the project. Pages do add or edit, should use navigaion, not modals. Should have breadcrumbs for all pages. Every page should display loading, error, empty, and success states for data fetching. Cancel button aligned on the left and save on the right. Always evict overflow on layouts. Avoid using scrollbars, evict overflow. The page should display 404 error if the route is not found. Always work with SCSS and if possible flex layout.

## Backend (Rust)
- **Structure:** Keep submodules isolated. Should apply clean architecture pattern.
- **Application Layer:** Expose Rest endpoints. Should use Axum create. Should use DTO structs for Json. A mapper trait should be implemented to convert DTOs to domain and domain to DTOs. All endpoints should be documented using openapi annotations. Routes should be grouped by business logic. Get methods with Query params always return 200. Get methods with Path params always return 200 or 404. Post methods always return 201. Put/Patch/Delete methods always return 204.
- **Business Layer:** Should contain the business logic. Is Composed by folders, Usecases, gateway, domains, commons. Domain with place the business domain, gateway is a outbound layer. Can abstract database access or external apis. Usecases should receive the gateway traits and not the implementations. The usecases should use the services defined in the domain layer. Should be independent of external dependencies. Use the ? operator to propagate errors. Usecase cases should be logged always, Don't return Result<T, Error> or Ok(T). Will return Option<T> or Vec<T>.
- **Entity layer:** should keep the database entities. With ORM framework mapping, normally SeaORM. The entity layer should not contain any business logic. and should implement EntityMapper trait for convertion with domain.
- **Migraton:** should be simple and declarative. Should use the SeaORM migration framework.
- **Error Handling:** Using `.unwrap()` in production code is strictly prohibited. Always propagate errors using `Result` and the `?` operator.
- **Dependencies:** Do not add new crates to `Cargo.toml` without explicit permission.
- **Tests:** All tests should be placed in test folder. One file per struct, for example shipping.rs shipping_test.rs. coupon_use_case.rs should have a test file named coupon_use_case_test.rs
- **Clippy:** Always run `cargo clippy` for Rust changes, covering the affected workspace and relevant targets/features. Apply Clippy recommendations whenever they preserve correctness and project conventions; document why any warning is intentionally retained.


## Project Architecture
- **Microservices:** The project is divided into two microservices: frontend and backend.
- **Communication:** The frontend communicates with the backend using REST APIs.
- **Database:** The database is managed by the backend.
- **Deployment:** The project is deployed using Docker and Docker Compose.
- **Version Control:** The project uses Git for version control.