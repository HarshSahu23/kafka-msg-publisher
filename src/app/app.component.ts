import { Component, OnInit } from '@angular/core';
import { RouterOutlet, RouterLink, RouterLinkActive } from '@angular/router';

@Component({
  selector: 'app-root',
  standalone: true,
  imports: [RouterOutlet, RouterLink, RouterLinkActive],
  templateUrl: './app.component.html',
  styleUrl: './app.component.css',
})
export class AppComponent implements OnInit {
  isDarkMode = true;
  isDrawerCollapsed = false;

  ngOnInit() {
    // Restore drawer state
    const saved = localStorage.getItem('drawer-collapsed');
    if (saved !== null) this.isDrawerCollapsed = saved === 'true';

    // Apply saved theme (needed for all pages, not just publisher)
    const theme = localStorage.getItem('theme');
    if (theme === 'light') {
      this.isDarkMode = false;
      document.documentElement.setAttribute('data-theme', 'light');
    }
  }

  toggleTheme() {
    this.isDarkMode = !this.isDarkMode;
    const theme = this.isDarkMode ? 'dark' : 'light';
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('theme', theme);
  }

  toggleDrawer() {
    this.isDrawerCollapsed = !this.isDrawerCollapsed;
    localStorage.setItem('drawer-collapsed', String(this.isDrawerCollapsed));
  }
}
