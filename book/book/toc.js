// Populate the sidebar
//
// This is a script, and not included directly in the page, to control the total size of the book.
// The TOC contains an entry for each page, so if each page includes a copy of the TOC,
// the total size of the page becomes O(n**2).
class MDBookSidebarScrollbox extends HTMLElement {
    constructor() {
        super();
    }
    connectedCallback() {
        this.innerHTML = '<ol class="chapter"><li class="chapter-item expanded affix "><a href="introduction.html">Introduction</a></li><li class="chapter-item expanded affix "><li class="part-title">Getting Started</li><li class="chapter-item expanded "><a href="getting-started/installation.html"><strong aria-hidden="true">1.</strong> Installation</a></li><li class="chapter-item expanded "><a href="getting-started/first-image.html"><strong aria-hidden="true">2.</strong> Your First Image</a></li><li class="chapter-item expanded "><a href="getting-started/build-modes.html"><strong aria-hidden="true">3.</strong> Build Modes</a></li><li class="chapter-item expanded affix "><li class="part-title">The KDL Spec Language</li><li class="chapter-item expanded "><a href="spec/overview.html"><strong aria-hidden="true">4.</strong> Spec Overview</a></li><li class="chapter-item expanded "><a href="spec/metadata.html"><strong aria-hidden="true">5.</strong> Metadata &amp; Distro</a></li><li class="chapter-item expanded "><a href="spec/repositories.html"><strong aria-hidden="true">6.</strong> Repositories &amp; Publishers</a></li><li class="chapter-item expanded "><a href="spec/packages.html"><strong aria-hidden="true">7.</strong> Packages</a></li><li class="chapter-item expanded "><a href="spec/overlays.html"><strong aria-hidden="true">8.</strong> Overlays</a></li><li class="chapter-item expanded "><a href="spec/customizations.html"><strong aria-hidden="true">9.</strong> Customizations</a></li><li class="chapter-item expanded "><a href="spec/targets.html"><strong aria-hidden="true">10.</strong> Targets</a></li><li class="chapter-item expanded "><a href="spec/builder.html"><strong aria-hidden="true">11.</strong> Builder Configuration</a></li><li class="chapter-item expanded affix "><li class="part-title">Composability</li><li class="chapter-item expanded "><a href="composability/base.html"><strong aria-hidden="true">12.</strong> Base Specs (Build Caching)</a></li><li class="chapter-item expanded "><a href="composability/includes.html"><strong aria-hidden="true">13.</strong> Includes (Shared Steps)</a></li><li class="chapter-item expanded "><a href="composability/profiles.html"><strong aria-hidden="true">14.</strong> Profiles (Conditional Variants)</a></li><li class="chapter-item expanded "><a href="composability/pipelines.html"><strong aria-hidden="true">15.</strong> Multi-Stage Pipelines</a></li><li class="chapter-item expanded affix "><li class="part-title">Distro Guide</li><li class="chapter-item expanded "><a href="distros/illumos-overview.html"><strong aria-hidden="true">16.</strong> illumos Overview</a></li><li class="chapter-item expanded "><a href="distros/omnios.html"><strong aria-hidden="true">17.</strong> OmniOS</a></li><li class="chapter-item expanded "><a href="distros/ubuntu.html"><strong aria-hidden="true">18.</strong> Ubuntu</a></li><li class="chapter-item expanded "><a href="distros/adding-distro.html"><strong aria-hidden="true">19.</strong> Adding a New Distro</a></li><li class="chapter-item expanded affix "><li class="part-title">Output Formats</li><li class="chapter-item expanded "><a href="formats/qcow2.html"><strong aria-hidden="true">20.</strong> QCOW2 VM Images</a></li><li class="chapter-item expanded "><a href="formats/oci.html"><strong aria-hidden="true">21.</strong> OCI Container Images</a></li><li class="chapter-item expanded "><a href="formats/artifact.html"><strong aria-hidden="true">22.</strong> Tar Artifacts</a></li><li class="chapter-item expanded "><a href="formats/registry.html"><strong aria-hidden="true">23.</strong> OCI Registry Push</a></li><li class="chapter-item expanded affix "><li class="part-title">Architecture</li><li class="chapter-item expanded "><a href="architecture/overview.html"><strong aria-hidden="true">24.</strong> Design Overview</a></li><li class="chapter-item expanded "><a href="architecture/pipeline.html"><strong aria-hidden="true">25.</strong> Two-Phase Build Pipeline</a></li><li class="chapter-item expanded "><a href="architecture/crates.html"><strong aria-hidden="true">26.</strong> Crate Structure</a></li><li class="chapter-item expanded "><a href="architecture/builder-vm.html"><strong aria-hidden="true">27.</strong> Remote Builder VMs</a></li><li class="chapter-item expanded affix "><li class="part-title">Migration</li><li class="chapter-item expanded "><a href="migration/from-image-builder.html"><strong aria-hidden="true">28.</strong> From omnios-image-builder</a></li><li class="chapter-item expanded "><a href="migration/from-packer.html"><strong aria-hidden="true">29.</strong> From Packer (oi-packer)</a></li><li class="chapter-item expanded affix "><li class="part-title">Reference</li><li class="chapter-item expanded "><a href="reference/cli.html"><strong aria-hidden="true">30.</strong> CLI Reference</a></li><li class="chapter-item expanded "><a href="reference/spec-reference.html"><strong aria-hidden="true">31.</strong> KDL Spec Reference</a></li><li class="chapter-item expanded "><a href="reference/examples.html"><strong aria-hidden="true">32.</strong> Example Specs</a></li></ol>';
        // Set the current, active page, and reveal it if it's hidden
        let current_page = document.location.href.toString().split("#")[0].split("?")[0];
        if (current_page.endsWith("/")) {
            current_page += "index.html";
        }
        var links = Array.prototype.slice.call(this.querySelectorAll("a"));
        var l = links.length;
        for (var i = 0; i < l; ++i) {
            var link = links[i];
            var href = link.getAttribute("href");
            if (href && !href.startsWith("#") && !/^(?:[a-z+]+:)?\/\//.test(href)) {
                link.href = path_to_root + href;
            }
            // The "index" page is supposed to alias the first chapter in the book.
            if (link.href === current_page || (i === 0 && path_to_root === "" && current_page.endsWith("/index.html"))) {
                link.classList.add("active");
                var parent = link.parentElement;
                if (parent && parent.classList.contains("chapter-item")) {
                    parent.classList.add("expanded");
                }
                while (parent) {
                    if (parent.tagName === "LI" && parent.previousElementSibling) {
                        if (parent.previousElementSibling.classList.contains("chapter-item")) {
                            parent.previousElementSibling.classList.add("expanded");
                        }
                    }
                    parent = parent.parentElement;
                }
            }
        }
        // Track and set sidebar scroll position
        this.addEventListener('click', function(e) {
            if (e.target.tagName === 'A') {
                sessionStorage.setItem('sidebar-scroll', this.scrollTop);
            }
        }, { passive: true });
        var sidebarScrollTop = sessionStorage.getItem('sidebar-scroll');
        sessionStorage.removeItem('sidebar-scroll');
        if (sidebarScrollTop) {
            // preserve sidebar scroll position when navigating via links within sidebar
            this.scrollTop = sidebarScrollTop;
        } else {
            // scroll sidebar to current active section when navigating via "next/previous chapter" buttons
            var activeSection = document.querySelector('#sidebar .active');
            if (activeSection) {
                activeSection.scrollIntoView({ block: 'center' });
            }
        }
        // Toggle buttons
        var sidebarAnchorToggles = document.querySelectorAll('#sidebar a.toggle');
        function toggleSection(ev) {
            ev.currentTarget.parentElement.classList.toggle('expanded');
        }
        Array.from(sidebarAnchorToggles).forEach(function (el) {
            el.addEventListener('click', toggleSection);
        });
    }
}
window.customElements.define("mdbook-sidebar-scrollbox", MDBookSidebarScrollbox);
